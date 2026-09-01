part of 'generated.dart';

class AddCommandHistoryVariablesBuilder {
  String cmd;
  Optional<String> _tag = Optional.optional(nativeFromJson, nativeToJson);

  final FirebaseDataConnect _dataConnect;  AddCommandHistoryVariablesBuilder tag(String? t) {
   _tag.value = t;
   return this;
  }

  AddCommandHistoryVariablesBuilder(this._dataConnect, {required  this.cmd,});
  Deserializer<AddCommandHistoryData> dataDeserializer = (dynamic json)  => AddCommandHistoryData.fromJson(jsonDecode(json));
  Serializer<AddCommandHistoryVariables> varsSerializer = (AddCommandHistoryVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<AddCommandHistoryData, AddCommandHistoryVariables>> execute() {
    return ref().execute();
  }

  MutationRef<AddCommandHistoryData, AddCommandHistoryVariables> ref() {
    AddCommandHistoryVariables vars= AddCommandHistoryVariables(cmd: cmd,tag: _tag,);
    return _dataConnect.mutation("AddCommandHistory", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class AddCommandHistoryCommandHistoryInsert {
  final String id;
  AddCommandHistoryCommandHistoryInsert.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final AddCommandHistoryCommandHistoryInsert otherTyped = other as AddCommandHistoryCommandHistoryInsert;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  AddCommandHistoryCommandHistoryInsert({
    required this.id,
  });
}

@immutable
class AddCommandHistoryData {
  final AddCommandHistoryCommandHistoryInsert commandHistory_insert;
  AddCommandHistoryData.fromJson(dynamic json):
  
  commandHistory_insert = AddCommandHistoryCommandHistoryInsert.fromJson(json['commandHistory_insert']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final AddCommandHistoryData otherTyped = other as AddCommandHistoryData;
    return commandHistory_insert == otherTyped.commandHistory_insert;
    
  }
  @override
  int get hashCode => commandHistory_insert.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['commandHistory_insert'] = commandHistory_insert.toJson();
    return json;
  }

  AddCommandHistoryData({
    required this.commandHistory_insert,
  });
}

@immutable
class AddCommandHistoryVariables {
  final String cmd;
  late final Optional<String>tag;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  AddCommandHistoryVariables.fromJson(Map<String, dynamic> json):
  
  cmd = nativeFromJson<String>(json['cmd']) {
  
  
  
    tag = Optional.optional(nativeFromJson, nativeToJson);
    tag.value = json['tag'] == null ? null : nativeFromJson<String>(json['tag']);
  
  }
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final AddCommandHistoryVariables otherTyped = other as AddCommandHistoryVariables;
    return cmd == otherTyped.cmd && 
    tag == otherTyped.tag;
    
  }
  @override
  int get hashCode => Object.hashAll([cmd.hashCode, tag.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['cmd'] = nativeToJson<String>(cmd);
    if(tag.state == OptionalState.set) {
      json['tag'] = tag.toJson();
    }
    return json;
  }

  AddCommandHistoryVariables({
    required this.cmd,
    required this.tag,
  });
}

