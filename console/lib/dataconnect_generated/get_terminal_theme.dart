part of 'generated.dart';

class GetTerminalThemeVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  GetTerminalThemeVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<GetTerminalThemeData> dataDeserializer = (dynamic json)  => GetTerminalThemeData.fromJson(jsonDecode(json));
  Serializer<GetTerminalThemeVariables> varsSerializer = (GetTerminalThemeVariables vars) => jsonEncode(vars.toJson());
  Future<QueryResult<GetTerminalThemeData, GetTerminalThemeVariables>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<GetTerminalThemeData, GetTerminalThemeVariables> ref() {
    GetTerminalThemeVariables vars= GetTerminalThemeVariables(id: id,);
    return _dataConnect.query("GetTerminalTheme", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class GetTerminalThemeTerminalTheme {
  final String name;
  final String colorPalette;
  GetTerminalThemeTerminalTheme.fromJson(dynamic json):
  
  name = nativeFromJson<String>(json['name']),
  colorPalette = nativeFromJson<String>(json['colorPalette']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetTerminalThemeTerminalTheme otherTyped = other as GetTerminalThemeTerminalTheme;
    return name == otherTyped.name && 
    colorPalette == otherTyped.colorPalette;
    
  }
  @override
  int get hashCode => Object.hashAll([name.hashCode, colorPalette.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    json['colorPalette'] = nativeToJson<String>(colorPalette);
    return json;
  }

  GetTerminalThemeTerminalTheme({
    required this.name,
    required this.colorPalette,
  });
}

@immutable
class GetTerminalThemeData {
  final GetTerminalThemeTerminalTheme? terminalTheme;
  GetTerminalThemeData.fromJson(dynamic json):
  
  terminalTheme = json['terminalTheme'] == null ? null : GetTerminalThemeTerminalTheme.fromJson(json['terminalTheme']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetTerminalThemeData otherTyped = other as GetTerminalThemeData;
    return terminalTheme == otherTyped.terminalTheme;
    
  }
  @override
  int get hashCode => terminalTheme.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (terminalTheme != null) {
      json['terminalTheme'] = terminalTheme!.toJson();
    }
    return json;
  }

  GetTerminalThemeData({
    this.terminalTheme,
  });
}

@immutable
class GetTerminalThemeVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  GetTerminalThemeVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final GetTerminalThemeVariables otherTyped = other as GetTerminalThemeVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  GetTerminalThemeVariables({
    required this.id,
  });
}

