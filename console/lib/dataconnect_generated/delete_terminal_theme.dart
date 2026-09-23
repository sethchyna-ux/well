part of 'generated.dart';

class DeleteTerminalThemeVariablesBuilder {
  String id;

  final FirebaseDataConnect _dataConnect;
  DeleteTerminalThemeVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<DeleteTerminalThemeData> dataDeserializer = (dynamic json)  => DeleteTerminalThemeData.fromJson(jsonDecode(json));
  Serializer<DeleteTerminalThemeVariables> varsSerializer = (DeleteTerminalThemeVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<DeleteTerminalThemeData, DeleteTerminalThemeVariables>> execute() {
    return ref().execute();
  }

  MutationRef<DeleteTerminalThemeData, DeleteTerminalThemeVariables> ref() {
    DeleteTerminalThemeVariables vars= DeleteTerminalThemeVariables(id: id,);
    return _dataConnect.mutation("DeleteTerminalTheme", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class DeleteTerminalThemeTerminalThemeDelete {
  final String id;
  DeleteTerminalThemeTerminalThemeDelete.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteTerminalThemeTerminalThemeDelete otherTyped = other as DeleteTerminalThemeTerminalThemeDelete;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteTerminalThemeTerminalThemeDelete({
    required this.id,
  });
}

@immutable
class DeleteTerminalThemeData {
  final DeleteTerminalThemeTerminalThemeDelete? terminalTheme_delete;
  DeleteTerminalThemeData.fromJson(dynamic json):
  
  terminalTheme_delete = json['terminalTheme_delete'] == null ? null : DeleteTerminalThemeTerminalThemeDelete.fromJson(json['terminalTheme_delete']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteTerminalThemeData otherTyped = other as DeleteTerminalThemeData;
    return terminalTheme_delete == otherTyped.terminalTheme_delete;
    
  }
  @override
  int get hashCode => terminalTheme_delete.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (terminalTheme_delete != null) {
      json['terminalTheme_delete'] = terminalTheme_delete!.toJson();
    }
    return json;
  }

  DeleteTerminalThemeData({
    this.terminalTheme_delete,
  });
}

@immutable
class DeleteTerminalThemeVariables {
  final String id;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  DeleteTerminalThemeVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final DeleteTerminalThemeVariables otherTyped = other as DeleteTerminalThemeVariables;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  DeleteTerminalThemeVariables({
    required this.id,
  });
}

